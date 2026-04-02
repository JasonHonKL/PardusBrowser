use crate::semantic::tree::TreeStats;

/// Format the stats summary line.
pub fn format_stats(stats: &TreeStats) -> String {
    format!(
        "semantic tree ready — {} landmarks, {} links, {} headings, {} actions",
        stats.landmarks, stats.links, stats.headings, stats.actions,
    )
}
