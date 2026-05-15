use weave_core::conflict::MergeStats;

use crate::merge::FileMergeResult;

/// Format merge results as a markdown PR comment.
pub fn format_comment(file_results: &[FileMergeResult], total_stats: &MergeStats) -> String {
    let mut out = String::new();

    // Header
    if total_stats.has_conflicts() {
        out.push_str("### Weave: entity-level merge analysis\n\n");
        out.push_str(&format!(
            "Weave resolved **{}** entities automatically but **{}** conflict(s) remain.\n\n",
            total_stats.entities_both_changed_merged, total_stats.entities_conflicted
        ));
    } else if total_stats.entities_both_changed_merged > 0 {
        out.push_str("### Weave: all conflicts resolved\n\n");
        out.push_str(&format!(
            "Weave resolved **{}** entities that were modified on both branches.\n\n",
            total_stats.entities_both_changed_merged
        ));
    } else {
        out.push_str("### Weave: no entity conflicts\n\n");
        out.push_str("No entities were modified on both branches.\n\n");
    }

    // Stats summary
    out.push_str(&format!("**Confidence:** {}\n\n", total_stats.confidence()));

    // Per-file breakdown
    if file_results.len() > 1 || !file_results.is_empty() {
        out.push_str("| File | Resolved | Conflicts | Confidence |\n");
        out.push_str("|------|----------|-----------|------------|\n");

        for fr in file_results {
            let resolved = fr.result.stats.entities_both_changed_merged;
            let conflicts = fr.result.stats.entities_conflicted;
            let confidence = fr.result.stats.confidence();

            out.push_str(&format!(
                "| `{}` | {} | {} | {} |\n",
                fr.path, resolved, conflicts, confidence
            ));
        }
        out.push('\n');
    }

    // Conflict details
    for fr in file_results {
        if fr.result.conflicts.is_empty() {
            continue;
        }

        out.push_str(&format!("#### `{}`\n\n", fr.path));

        for conflict in &fr.result.conflicts {
            out.push_str(&format!(
                "- **{} `{}`** ({}, {})\n",
                conflict.entity_type, conflict.entity_name, conflict.kind, conflict.complexity
            ));
            out.push_str(&format!("  > {}\n", conflict.complexity.resolution_hint()));
        }
        out.push('\n');
    }

    // Semantic warnings
    if total_stats.semantic_warnings > 0 {
        out.push_str(&format!(
            "**Warning:** {} auto-merged entities reference other modified entities. Review carefully.\n\n",
            total_stats.semantic_warnings
        ));
    }

    out
}
