//! On-demand explanation of ONE file's conflicts — the pull side of the
//! findings channel.
//!
//! An earlier design pushed a findings document beside every conflicted file
//! instead. Measured against this on-demand form, that document cost bytes on
//! every merge without moving the outcome, for two reasons:
//!
//! * it restated what the reader already had (three texts one `git show` away),
//!   and
//! * it counted **hunks**, not conflicts — "158 hunks, whole-file conflict" for
//!   three real disagreements is disorienting enough that a reader will
//!   reasonably re-derive the truth by hand with `git merge-file` instead of
//!   trusting it.
//!
//! So this module answers a *question*, not a broadcast, and it answers it
//! about the only hunks that are a question: the ones where **both sides
//! acted**. A hunk one side left alone is not a conflict; it is an edit that
//! happens to sit inside a box the other side's work opened. Filtering those
//! out is the whole difference between 158 and 3.
//!
//! Everything here is read off the merge weave already ran. Nothing is
//! inferred: the guard is the rung of the ladder that refused
//! ([`crate::ResolutionStrategy::guard`]), the hunks are
//! [`crate::diagnose`]'s alignment, and the repair — when there is one — is a
//! *typed* rewrite derivable from the conflict's own kind, never a sentence of
//! advice about what the developer probably meant.

use serde::Serialize;

use crate::conflict::{ConflictKind, EntityConflict, MarkerFormat};
use crate::diagnose::{Hunk, SideAction, SideEdit};
use crate::merge::entity_merge_fmt;

/// How many contested hunks one conflict may carry into the explanation.
/// Beyond this the count is still exact; the samples stop.
const HUNK_LIMIT: usize = 12;
/// How many lines of a side's edit are quoted.
const SAMPLE: usize = 3;
/// How wide a quoted line may be.
const WIDTH: usize = 100;

/// One file's conflicts, explained.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Explanation {
    pub file: String,
    /// True when the merge produced no conflict at all — the honest answer to
    /// "explain this file" is then a sentence, never an empty list.
    pub clean: bool,
    pub conflicts: Vec<ConflictExplanation>,
}

/// One conflicted entity.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ConflictExplanation {
    pub entity: String,
    pub entity_type: String,
    pub kind: &'static str,
    /// Which guard refused. The same name the marker prints, from the same
    /// owner, so the artifact and the explanation cannot disagree.
    pub refused_by: &'static str,
    pub complexity: String,
    /// How many hunks BOTH sides touched. This is the conflict count a reader
    /// is entitled to; it is not the number of diff hunks in the entity.
    pub contested_hunks: usize,
    /// The first [`HUNK_LIMIT`] of them, in base order.
    pub hunks: Vec<HunkExplanation>,
    /// A mechanical rewrite that follows from the conflict's kind, when one
    /// does. `None` means weave has nothing derivable to offer and says so,
    /// rather than filling the field with an empty placeholder a reader has
    /// to notice is empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repair: Option<Repair>,
}

/// A rewrite that is *derivable*, stated as data rather than prose.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "repair", rename_all = "snake_case")]
pub enum Repair {
    /// One side renamed the declaration and the other edited its body: adopt
    /// the new name, replay the body edit onto it, rewrite the call sites.
    Rename { from: String, to: String },
    /// One side deleted the declaration and the other kept editing it.
    DecideExistence { kept_by: &'static str },
    /// Both sides introduced the same name independently.
    Reconcile { name: String },
}

impl std::fmt::Display for Repair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Repair::Rename { from, to } => write!(
                f,
                "rename `{from}` -> `{to}` in the resolution, replay the body edit onto \
                 the new name, then rewrite every call site of `{from}`"
            ),
            Repair::DecideExistence { kept_by } => write!(
                f,
                "deleted on one side, still edited by {kept_by} — decide whether it has a \
                 reason to exist before resolving"
            ),
            Repair::Reconcile { name } => write!(
                f,
                "`{name}` was added independently by both sides — fold the two definitions \
                 into one, do not keep both"
            ),
        }
    }
}

/// One place inside a conflict where both sides wrote.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct HunkExplanation {
    /// 1-based inclusive base lines this hunk replaces, inside the entity's own
    /// base text. `base_end < base_start` is a pure insertion at `base_start`.
    pub base_start: usize,
    pub base_end: usize,
    pub ours: SideSummary,
    pub theirs: SideSummary,
    /// Base lines BOTH sides deleted or rewrote — the actual disagreement.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub collision: Vec<String>,
}

/// What one side did here.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SideSummary {
    pub action: &'static str,
    pub removed_lines: usize,
    pub added_lines: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub added: Vec<String>,
}

impl SideSummary {
    fn of(e: &SideEdit) -> Self {
        SideSummary {
            action: e.action.wire(),
            removed_lines: e.removed.len(),
            added_lines: e.added.len(),
            added: e.added.iter().take(SAMPLE).map(|l| clip(l)).collect(),
        }
    }
}

fn clip(line: &str) -> String {
    let t = line.trim_end();
    if t.chars().count() > WIDTH {
        t.chars().take(WIDTH - 1).chain(['…']).collect()
    } else {
        t.to_string()
    }
}

/// Is this hunk a question? Only when both sides wrote in it.
fn contested(h: &Hunk) -> bool {
    h.ours.action != SideAction::Untouched && h.theirs.action != SideAction::Untouched
}

fn repair_of(c: &EntityConflict) -> Option<Repair> {
    match &c.kind {
        ConflictKind::RenameModify {
            old_name, new_name, ..
        } => Some(Repair::Rename {
            from: old_name.clone(),
            to: new_name.clone(),
        }),
        ConflictKind::RenameRename {
            base_name,
            ours_name,
            ..
        } => Some(Repair::Rename {
            from: base_name.clone(),
            to: ours_name.clone(),
        }),
        ConflictKind::ModifyDelete { modified_in_ours } => Some(Repair::DecideExistence {
            kept_by: if *modified_in_ours { "ours" } else { "theirs" },
        }),
        ConflictKind::BothAdded => Some(Repair::Reconcile {
            name: c.entity_name.clone(),
        }),
        // Both sides rewrote the same body. There is no rewrite that follows
        // from that fact, and inventing one is the thing a merge tool may not
        // do. The hunks below are the answer.
        ConflictKind::BothModified => None,
    }
}

/// Merge the triple and explain what refused, for one file.
///
/// The merge is re-run rather than cached: `explain` is asked at most once per
/// file, by a human or an agent that has already decided this file is worth a
/// question, and re-running it is what makes the explanation about the bytes in
/// front of the reader rather than about a snapshot taken earlier.
pub fn explain(
    base: &str,
    ours: &str,
    theirs: &str,
    file_path: &str,
    host: &crate::host::Host,
) -> Explanation {
    let result = entity_merge_fmt(
        base,
        ours,
        theirs,
        file_path,
        &MarkerFormat::default(),
        host,
    );
    // Entity name -> the guard that refused it. The audit is the only record of
    // which rung declined; joining on the name is exact here because the audit
    // carries one row per merged triple and a conflict IS one of those triples.
    let guard_of = |name: &str, entity_type: &str| -> &'static str {
        result
            .audit
            .iter()
            .find(|a| a.name == name && a.entity_type == entity_type)
            .and_then(|a| a.resolution.guard())
            .unwrap_or("merge_ladder_exhausted")
    };
    let conflicts = result
        .conflicts
        .iter()
        .map(|c| {
            let hunks: Vec<Hunk> = match (
                c.base_content.as_deref(),
                c.ours_content.as_deref(),
                c.theirs_content.as_deref(),
            ) {
                (Some(b), Some(o), Some(t)) => crate::diagnose::diagnose(b, o, t)
                    .into_iter()
                    .filter(contested)
                    .collect(),
                _ => Vec::new(),
            };
            ConflictExplanation {
                entity: c.entity_name.clone(),
                entity_type: c.entity_type.clone(),
                kind: c.kind.wire_kind(),
                refused_by: guard_of(&c.entity_name, &c.entity_type),
                complexity: c.complexity.to_string(),
                contested_hunks: hunks.len(),
                hunks: hunks
                    .iter()
                    .take(HUNK_LIMIT)
                    .map(|h| HunkExplanation {
                        base_start: h.base_start,
                        base_end: h.base_end,
                        ours: SideSummary::of(&h.ours),
                        theirs: SideSummary::of(&h.theirs),
                        collision: h.collision.iter().take(SAMPLE).map(|l| clip(l)).collect(),
                    })
                    .collect(),
                repair: repair_of(c),
            }
        })
        .collect();
    Explanation {
        file: file_path.to_string(),
        clean: result.is_clean(),
        conflicts,
    }
}

impl Explanation {
    /// The explanation as the lines a reader gets on a terminal.
    ///
    /// A slim, flat rendering is the point of the whole module: the JSON exists
    /// for programs, and this exists so that reading it costs less than
    /// re-deriving it with `git show :1: / :2: / :3:`.
    ///
    /// **Never empty.** An explanation with no conflicts prints a sentence
    /// saying so — a reader who gets a bare `[]` can mistake absence of
    /// output for approval; a channel that can print nothing has a failure
    /// mode where silence is indistinguishable from a clean bill of health.
    pub fn render(&self) -> String {
        let mut out = String::new();
        if self.conflicts.is_empty() {
            out.push_str(&format!(
                "{}: weave's merge of this file is CLEAN — no conflicted entity, \
                 nothing to explain.\n",
                self.file
            ));
            return out;
        }
        out.push_str(&format!(
            "{}: {} conflicted entit{}\n",
            self.file,
            self.conflicts.len(),
            if self.conflicts.len() == 1 {
                "y"
            } else {
                "ies"
            }
        ));
        for c in &self.conflicts {
            out.push_str(&format!(
                "\n{} `{}` — {} ({}), refused_by: {}\n",
                c.entity_type, c.entity, c.kind, c.complexity, c.refused_by
            ));
            if let Some(r) = &c.repair {
                out.push_str(&format!("  repair: {r}\n"));
            }
            if c.contested_hunks == 0 {
                out.push_str(
                    "  no hunk has edits from both sides — the two sides' work does not \
                     overlap inside this entity\n",
                );
                continue;
            }
            out.push_str(&format!(
                "  {} contested hunk(s) (hunks BOTH sides wrote in; one-sided hunks are \
                 not conflicts and are not listed)\n",
                c.contested_hunks
            ));
            for (i, h) in c.hunks.iter().enumerate() {
                let span = if h.base_end < h.base_start {
                    format!("insert before base line {}", h.base_start)
                } else {
                    format!("base lines {}-{}", h.base_start, h.base_end)
                };
                out.push_str(&format!(
                    "  [{}] {} — ours {} (-{}/+{}), theirs {} (-{}/+{})\n",
                    i + 1,
                    span,
                    h.ours.action,
                    h.ours.removed_lines,
                    h.ours.added_lines,
                    h.theirs.action,
                    h.theirs.removed_lines,
                    h.theirs.added_lines
                ));
                for l in &h.collision {
                    out.push_str(&format!("      both rewrote: {l}\n"));
                }
                for l in &h.ours.added {
                    out.push_str(&format!("      ours   +{l}\n"));
                }
                for l in &h.theirs.added {
                    out.push_str(&format!("      theirs +{l}\n"));
                }
            }
            if c.contested_hunks > c.hunks.len() {
                out.push_str(&format!(
                    "  … {} more contested hunk(s) not shown\n",
                    c.contested_hunks - c.hunks.len()
                ));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "def a():\n    x = 1\n    return x\n\ndef b():\n    return 2\n";

    #[test]
    fn only_hunks_both_sides_wrote_in_are_counted() {
        // ours edits a() and b(); theirs edits only a(). b() is one-sided, so
        // it is not a conflict and must not appear as one.
        let ours = "def a():\n    x = 11\n    return x\n\ndef b():\n    return 22\n";
        let theirs = "def a():\n    x = 33\n    return x\n\ndef b():\n    return 2\n";
        let e = explain(BASE, ours, theirs, "m.py", &crate::host::Host::default());
        assert!(!e.clean);
        assert_eq!(e.conflicts.len(), 1, "{e:#?}");
        let c = &e.conflicts[0];
        assert_eq!(c.entity, "a");
        assert_eq!(c.contested_hunks, 1, "{c:#?}");
        assert_eq!(c.hunks[0].collision, vec!["    x = 1".to_string()]);
    }

    #[test]
    fn a_clean_merge_explains_itself_in_a_sentence_and_never_an_empty_list() {
        let ours = "def a():\n    x = 11\n    return x\n\ndef b():\n    return 2\n";
        let theirs = "def a():\n    x = 1\n    return x\n\ndef b():\n    return 22\n";
        let e = explain(BASE, ours, theirs, "m.py", &crate::host::Host::default());
        assert!(e.clean, "{e:#?}");
        let r = e.render();
        assert!(r.contains("CLEAN"), "{r}");
        assert!(!r.trim().is_empty());
    }

    #[test]
    fn the_guard_named_here_is_the_guard_the_marker_prints() {
        let ours = "def a():\n    x = 11\n    return x\n\ndef b():\n    return 2\n";
        let theirs = "def a():\n    x = 33\n    return x\n\ndef b():\n    return 2\n";
        let e = explain(BASE, ours, theirs, "m.py", &crate::host::Host::default());
        let merged = entity_merge_fmt(
            BASE,
            ours,
            theirs,
            "m.py",
            &MarkerFormat::default(),
            &crate::host::Host::default(),
        );
        let guard = e.conflicts[0].refused_by;
        assert!(
            merged.content.contains(&format!("refused_by: {guard}")),
            "explain says {guard}, artifact says:\n{}",
            merged.content
        );
    }

    #[test]
    fn a_rename_carries_a_derivable_repair() {
        let base = "def fetch():\n    return 1\n";
        let ours = "def get():\n    return 1\n";
        let theirs = "def fetch():\n    return 99\n";
        let e = explain(base, ours, theirs, "m.py", &crate::host::Host::default());
        let repair = e.conflicts.iter().find_map(|c| c.repair.clone());
        assert!(
            matches!(&repair, Some(Repair::Rename { from, to }) if from == "fetch" && to == "get"),
            "{e:#?}"
        );
    }
}
