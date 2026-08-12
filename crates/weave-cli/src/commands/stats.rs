use weave_core::stats::WeaveLifetimeStats;

pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok();
    let stats = match weave_core::stats::default_path(home.as_deref()) {
        Some(path) => WeaveLifetimeStats::load(&path),
        None => WeaveLifetimeStats::default(),
    };

    if stats.total_merges == 0 {
        // Recording is opt-in: the merge driver does no filesystem work beyond
        // the files it was handed unless asked to, so an empty counter usually
        // means "never switched on", not "never merged".
        println!("No weave merges recorded yet.");
        println!("Recording is off by default — set WEAVE_STATS=1 in the environment");
        println!("git or jj runs weave in, and counters will accumulate from the next merge.");
        return Ok(());
    }

    let total_resolved = stats.conflicts_auto_resolved + stats.conflicts_unresolved;
    let success_rate = if total_resolved > 0 {
        (stats.conflicts_auto_resolved as f64 / total_resolved as f64 * 100.0) as u64
    } else {
        100
    };

    println!();
    println!("  weave lifetime stats");
    println!("  {}", "─".repeat(36));
    println!("  {:>8}  total merges", stats.total_merges);
    println!(
        "  {:>8}  conflicts auto-resolved",
        stats.conflicts_auto_resolved
    );
    println!("  {:>8}  conflicts remaining", stats.conflicts_unresolved);
    println!();
    println!(
        "  -> {} merges, {} auto-resolved ({}% success rate)",
        stats.total_merges, stats.conflicts_auto_resolved, success_rate
    );
    println!();

    Ok(())
}
