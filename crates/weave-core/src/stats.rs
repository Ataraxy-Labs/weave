use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::conflict::MergeStats;

#[derive(Serialize, Deserialize, Default)]
pub struct WeaveLifetimeStats {
    pub version: u32,
    pub first_run: Option<String>,
    pub last_run: Option<String>,
    pub total_merges: u64,
    pub total_entities_processed: u64,
    pub conflicts_auto_resolved: u64,
    pub conflicts_unresolved: u64,
    pub confidence_very_high: u64,
    pub confidence_high: u64,
    pub confidence_medium: u64,
    pub confidence_conflict: u64,
}

fn stats_path() -> Option<PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()?;
    Some(PathBuf::from(home).join(".weave").join("stats.json"))
}

fn now_iso() -> String {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    // Simple UTC timestamp: seconds since epoch -> YYYY-MM-DDTHH:MM:SSZ
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let h = time_of_day / 3600;
    let m = (time_of_day % 3600) / 60;
    let s = time_of_day % 60;

    // Days since 1970-01-01
    let mut y = 1970i64;
    let mut remaining = days as i64;
    loop {
        let days_in_year = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
            366
        } else {
            365
        };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let month_days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut mon = 0;
    for (i, &md) in month_days.iter().enumerate() {
        if remaining < md as i64 {
            mon = i;
            break;
        }
        remaining -= md as i64;
    }
    let day = remaining + 1;
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        mon + 1,
        day,
        h,
        m,
        s
    )
}

impl WeaveLifetimeStats {
    pub fn load() -> Self {
        let path = match stats_path() {
            Some(p) => p,
            None => return Self::default(),
        };
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn record_merge(mut self, stats: &MergeStats) -> Self {
        let now = now_iso();
        if self.first_run.is_none() {
            self.first_run = Some(now.clone());
        }
        self.last_run = Some(now);
        self.version = 1;

        self.total_merges += 1;

        let auto_resolved = stats.entities_ours_only
            + stats.entities_theirs_only
            + stats.entities_both_changed_merged;
        let total_entities = auto_resolved
            + stats.entities_unchanged
            + stats.entities_conflicted
            + stats.entities_added_ours
            + stats.entities_added_theirs
            + stats.entities_deleted;

        self.total_entities_processed += total_entities as u64;
        self.conflicts_auto_resolved += auto_resolved as u64;
        self.conflicts_unresolved += stats.entities_conflicted as u64;

        match stats.confidence() {
            "very_high" => self.confidence_very_high += 1,
            "high" => self.confidence_high += 1,
            "medium" => self.confidence_medium += 1,
            "conflict" => self.confidence_conflict += 1,
            _ => {}
        }

        self
    }

    pub fn save(self) -> Self {
        if let Some(path) = stats_path() {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(&self) {
                let _ = fs::write(&path, json);
            }
        }
        self
    }
}
