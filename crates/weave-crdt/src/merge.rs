use std::collections::HashMap;

use serde::Serialize;

/// Vector clock for tracking causal ordering of entity edits across agents.
///
/// Each agent has a counter. When an agent modifies an entity, its counter
/// is incremented. Two version vectors are concurrent (conflict) when neither
/// dominates the other.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct VersionVector {
    counters: HashMap<String, u64>,
}

impl VersionVector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Increment an agent's counter by 1.
    pub fn increment(&mut self, agent_id: &str) {
        let counter = self.counters.entry(agent_id.to_string()).or_insert(0);
        *counter += 1;
    }

    /// Component-wise max of two version vectors.
    pub fn merge(&mut self, other: &VersionVector) {
        for (agent, &count) in &other.counters {
            let entry = self.counters.entry(agent.clone()).or_insert(0);
            if count > *entry {
                *entry = count;
            }
        }
    }

    /// Compare two version vectors for causal ordering.
    ///
    /// Returns `Some(Ordering::Less)` if self is dominated by other,
    /// `Some(Ordering::Greater)` if self dominates other,
    /// `Some(Ordering::Equal)` if equal,
    /// `None` if concurrent (neither dominates).
    pub fn partial_cmp(&self, other: &VersionVector) -> Option<std::cmp::Ordering> {
        let all_keys: std::collections::HashSet<&String> =
            self.counters.keys().chain(other.counters.keys()).collect();

        let mut has_greater = false;
        let mut has_less = false;

        for key in all_keys {
            let ours = self.counters.get(key).copied().unwrap_or(0);
            let theirs = other.counters.get(key).copied().unwrap_or(0);

            if ours > theirs {
                has_greater = true;
            }
            if ours < theirs {
                has_less = true;
            }

            if has_greater && has_less {
                return None; // Concurrent
            }
        }

        match (has_greater, has_less) {
            (false, false) => Some(std::cmp::Ordering::Equal),
            (true, false) => Some(std::cmp::Ordering::Greater),
            (false, true) => Some(std::cmp::Ordering::Less),
            (true, true) => None, // Concurrent (already returned above, but for safety)
        }
    }

    /// Sum of all counters. Backward-compatible with the old scalar `version` field.
    pub fn total(&self) -> u64 {
        self.counters.values().sum()
    }

    /// Get the counter for a specific agent.
    pub fn get(&self, agent_id: &str) -> u64 {
        self.counters.get(agent_id).copied().unwrap_or(0)
    }

    /// Check if this vector is empty (all zeros / no entries).
    pub fn is_empty(&self) -> bool {
        self.counters.is_empty() || self.counters.values().all(|&v| v == 0)
    }

    /// Get all agent IDs in this vector.
    pub fn agents(&self) -> Vec<&String> {
        self.counters.keys().collect()
    }

    /// Get the raw counters map.
    pub fn counters(&self) -> &HashMap<String, u64> {
        &self.counters
    }

    /// Create from a HashMap.
    pub fn from_map(map: HashMap<String, u64>) -> Self {
        Self { counters: map }
    }
}

/// The merge state of an entity in the CRDT.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum MergeState {
    Clean,
    Conflict {
        ours: String,
        theirs: String,
        base: String,
        ours_agent: String,
        theirs_agent: String,
    },
}

impl MergeState {
    pub fn as_str(&self) -> &str {
        match self {
            MergeState::Clean => "clean",
            MergeState::Conflict { .. } => "conflict",
        }
    }
}

/// Result of merging all entities in a file via the CRDT.
#[derive(Debug, Clone, Serialize)]
pub struct CrdtMergeResult {
    pub file_path: String,
    pub entities_auto_merged: usize,
    pub entities_conflicted: usize,
    pub merged_content: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vv_increment() {
        let mut vv = VersionVector::new();
        assert_eq!(vv.total(), 0);
        vv.increment("agent-1");
        assert_eq!(vv.get("agent-1"), 1);
        assert_eq!(vv.total(), 1);
        vv.increment("agent-1");
        assert_eq!(vv.get("agent-1"), 2);
        assert_eq!(vv.total(), 2);
    }

    #[test]
    fn test_vv_merge_max() {
        let mut a = VersionVector::new();
        a.increment("agent-1");
        a.increment("agent-1");
        a.increment("agent-2");

        let mut b = VersionVector::new();
        b.increment("agent-1");
        b.increment("agent-2");
        b.increment("agent-2");
        b.increment("agent-3");

        a.merge(&b);
        assert_eq!(a.get("agent-1"), 2); // max(2, 1)
        assert_eq!(a.get("agent-2"), 2); // max(1, 2)
        assert_eq!(a.get("agent-3"), 1); // max(0, 1)
    }

    #[test]
    fn test_vv_partial_cmp_dominated() {
        let mut a = VersionVector::new();
        a.increment("agent-1");

        let mut b = VersionVector::new();
        b.increment("agent-1");
        b.increment("agent-1");
        b.increment("agent-2");

        // a < b (b dominates a)
        assert_eq!(a.partial_cmp(&b), Some(std::cmp::Ordering::Less));
        assert_eq!(b.partial_cmp(&a), Some(std::cmp::Ordering::Greater));
    }

    #[test]
    fn test_vv_partial_cmp_concurrent() {
        let mut a = VersionVector::new();
        a.increment("agent-1");
        a.increment("agent-1");

        let mut b = VersionVector::new();
        b.increment("agent-2");
        b.increment("agent-2");

        // Neither dominates: concurrent
        assert_eq!(a.partial_cmp(&b), None);
        assert_eq!(b.partial_cmp(&a), None);
    }

    #[test]
    fn test_vv_partial_cmp_equal() {
        let mut a = VersionVector::new();
        a.increment("agent-1");

        let mut b = VersionVector::new();
        b.increment("agent-1");

        assert_eq!(a.partial_cmp(&b), Some(std::cmp::Ordering::Equal));
    }
}
