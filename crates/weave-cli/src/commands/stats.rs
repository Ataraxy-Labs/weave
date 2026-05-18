use weave_core::stats::WeaveLifetimeStats;

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let stats = WeaveLifetimeStats::load();

    if stats.total_merges == 0 {
        println!("No weave merges recorded yet. Run some merges first!");
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
