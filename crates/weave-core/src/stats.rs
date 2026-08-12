//! The lifetime merge counter behind `weave stats` — opt-in, and honest about
//! being opt-in.
//!
//! One file, `~/.weave/stats.json`, written by `weave-driver` only when
//! `WEAVE_STATS=1` is set, and read by the `weave stats` command. The default
//! merge path touches no file except the merge's own inputs and output, so an
//! empty counter almost always means "never switched on" rather than "never
//! merged" — which is why `weave stats` says so instead of printing zeros.
//!
//! *Where* the file lives is the caller's to decide, not this module's. It
//! used to read `HOME` itself, which meant a zero-argument `load()` reached the
//! environment and the disk while the `WEAVE_STATS` gate that was supposed to
//! protect it lived in another crate entirely. [`default_path`] still offers
//! the conventional location, but a caller has to ask for it and hand it
//! over — and a test can hand over somewhere else.
//!
//! It is deliberately a counter and not a log: no file paths, no entity names,
//! no timestamps beyond first and last run. Nothing here is consulted by any
//! merge decision.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
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

/// The conventional location, `~/.weave/stats.json`, or `None` when the home
/// directory cannot be determined.
///
/// This is the one function here that reads the environment, it is not on any
/// merge path, and the binaries call it themselves and pass the result down.
pub fn default_path(home: Option<&str>) -> Option<PathBuf> {
    Some(PathBuf::from(home?).join(".weave").join("stats.json"))
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
    /// Read the counter at `path`, or a zeroed one if it is absent or
    /// unreadable — the counter is advisory, and a missing file is the normal
    /// state.
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path)
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

        let auto_resolved = stats.auto_resolved();
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

    /// Write the counter to `path`, creating its directory if need be.
    ///
    /// Returns whether the write landed. It used to return `Self` and swallow
    /// both failures, so the driver's `let _ = ...` was discarding a result
    /// that never said anything in the first place.
    pub fn save(&self, path: &Path) -> bool {
        if let Some(parent) = path.parent() {
            if fs::create_dir_all(parent).is_err() {
                return false;
            }
        }
        match serde_json::to_string_pretty(self) {
            Ok(json) => fs::write(path, json).is_ok(),
            Err(_) => false,
        }
    }
}
