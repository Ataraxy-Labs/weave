//! Rendering: a fold over the plan. It decides nothing.
//!
//! v1's reconstruction chased predecessor chains, tracked which entities had
//! already been emitted, special-cased blank lines per entity shape, and then
//! ran `post_merge_cleanup` over the result to delete the duplicate lines it
//! had just produced. Every one of those was a consequence of placement being
//! decided *during* emission.
//!
//! Here placement is already decided, and every entity appears in the plan
//! exactly once (it came from a triple, and every entity is in exactly one
//! triple), so the fold emits each once and needs no clean-up pass afterwards.
//! `is_clean` likewise stops scanning the output for `<<<<<<<`: conflict status
//! is read from the dispositions.
//!
//! The interstitials get the same treatment as the entities: `extract_regions`
//! already tiles the file, so the merged gaps and the merged entities together
//! ARE the file. The fold emits each gap verbatim, exactly once, in front of
//! the entity it leads.
//!
//! That is a change of policy from "read the file's dominant blank-line
//! convention and apply it everywhere". That uniform convention was
//! byte-lossy whenever a file does not have a single dominant convention: a
//! file that separates some declarations by one blank line and some by two
//! has no single convention, and
//! rewriting it to one is a diff the merge invented. A synthesised separator is
//! now what happens only where an insertion creates a boundary that never
//! existed in any version, which is the only place there is nothing to preserve.

use std::collections::{HashMap, HashSet};

use super::plan::PlannedItem;
use super::resolve::Resolved;
use super::types::*;
use crate::region::FileRegion;

/// Interstitial text, already merged, keyed by the region position key.
pub(crate) struct Interstitials<'a> {
    pub merged: &'a HashMap<String, String>,
    /// next-entity source id → the interstitial KEYS that lead it, in the order
    /// the versions state them. Keys, not text, so the fold can guarantee each
    /// gap is emitted at most once.
    ///
    /// A gap whose following entity this merge does not emit is not thereby
    /// deleted: it rolls forward to the next entity that IS emitted, so a
    /// section comment survives the deletion of the declaration under it.
    lead_by_entity: HashMap<String, Vec<String>>,
    /// Keys no surviving entity leads — everything after the last one.
    trailing: Vec<String>,
    /// What to put between two declarations that were never adjacent in any
    /// version, read off the base.
    separator: String,
}

impl<'a> Interstitials<'a> {
    /// Index the merged interstitials by the entity each one leads, so a gap
    /// travels with the entity it belongs to when placement moves that entity.
    ///
    /// The index is built by walking the region lists rather than by parsing
    /// `between:<prev>:<next>` keys — sem-core ids contain `:` themselves, so
    /// the key is not a parseable pair. The regions know which entity follows
    /// each gap; that is where the fact lives.
    ///
    /// `emitted` is the set of source ids this merge will actually render.
    /// Walking the regions without it was how content disappeared: a gap in
    /// front of a deleted entity was indexed under an id nothing would ask for.
    pub(crate) fn new(
        merged: &'a HashMap<String, String>,
        regions: &[&[FileRegion]],
        emitted: &HashSet<String>,
    ) -> Self {
        let mut lead_by_entity: HashMap<String, Vec<String>> = HashMap::new();
        let mut trailing: Vec<String> = Vec::new();
        let mut assigned: HashSet<&str> = HashSet::new();
        assigned.insert("file_header");
        assigned.insert("file_footer");
        assigned.insert("file_only");
        for list in regions {
            let mut pending: Vec<&str> = Vec::new();
            for region in list.iter() {
                match region {
                    FileRegion::Interstitial(i) => {
                        let key = i.position_key.as_str();
                        // A gap the merge resolved to nothing states nothing.
                        if merged.get(key).is_some_and(|t| !t.is_empty()) && assigned.insert(key) {
                            pending.push(key);
                        }
                    }
                    FileRegion::Entity(e) => {
                        if emitted.contains(&e.entity_id) && !pending.is_empty() {
                            lead_by_entity
                                .entry(e.entity_id.clone())
                                .or_default()
                                .extend(pending.drain(..).map(str::to_string));
                        }
                    }
                }
            }
            trailing.extend(pending.into_iter().map(str::to_string));
        }
        let separator = regions
            .first()
            .map(|list| dominant_gap(list))
            .unwrap_or_else(|| "\n".to_string());
        Interstitials {
            merged,
            lead_by_entity,
            trailing,
            separator,
        }
    }

    /// The gap keys that lead this triple, under whichever of its three source
    /// ids the index knows.
    fn lead_keys(&self, triple: &Triple, arena: &Arena) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for idx in [triple.base_idx(), triple.ours_idx(), triple.theirs_idx()]
            .into_iter()
            .flatten()
        {
            if let Some(keys) = self.lead_by_entity.get(&arena.get(idx).src_id) {
                out.extend(keys.iter().map(String::as_str));
            }
        }
        out
    }
}

/// The gap that appears most often between the base file's top-level entities:
/// what this file does when it puts two declarations next to each other.
///
/// Two declarations with no interstitial region between them are an
/// observation too — the empty gap — and not counting it was how a file that
/// packs its declarations together came back with a blank line between every
/// pair. Ties break on the shorter gap, so the answer is a function of the file
/// and nothing else.
fn dominant_gap(regions: &[FileRegion]) -> String {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    let mut seen_entity = false;
    let mut gap_since_entity = false;
    for region in regions {
        match region {
            FileRegion::Entity(_) => {
                if seen_entity && !gap_since_entity {
                    *counts.entry("").or_insert(0) += 1;
                }
                seen_entity = true;
                gap_since_entity = false;
            }
            FileRegion::Interstitial(i) => {
                if seen_entity {
                    gap_since_entity = true;
                    if i.content.trim().is_empty() {
                        *counts.entry(i.content.as_str()).or_insert(0) += 1;
                    }
                }
            }
        }
    }
    // `max_by` over a HashMap returns the LAST maximum, so any pair the
    // comparator calls equal is decided by hash order. Two distinct
    // whitespace-only gaps of the SAME length and the same count is a real tie
    // ("\n\n" vs "\n\t"), so the comparator is made total on the gap text
    // itself — otherwise the separator between two never-adjacent declarations
    // is a function of the process's hash seed, which breaks determinism.
    counts
        .into_iter()
        .max_by(|a, b| {
            a.1.cmp(&b.1)
                .then_with(|| b.0.len().cmp(&a.0.len()))
                .then_with(|| b.0.cmp(a.0))
        })
        .map(|(gap, _)| gap.to_string())
        .unwrap_or_else(|| "\n".to_string())
}

/// Every source id the plan will emit, in every version. `Interstitials` needs
/// it before it can say which entity a gap leads.
pub(crate) fn emitted_src_ids(
    arena: &Arena,
    triples: &[Triple],
    resolved: &[Resolved],
    items: &[PlannedItem],
) -> HashSet<String> {
    let mut out = HashSet::new();
    for item in items {
        if matches!(resolved[item.triple].disposition, Disposition::Drop) {
            continue;
        }
        let triple = &triples[item.triple];
        for idx in [triple.base_idx(), triple.ours_idx(), triple.theirs_idx()]
            .into_iter()
            .flatten()
        {
            out.insert(arena.get(idx).src_id.clone());
        }
    }
    out
}

pub(crate) fn render(
    arena: &Arena,
    triples: &[Triple],
    resolved: &[Resolved],
    items: &[PlannedItem],
    interstitials: &Interstitials<'_>,
) -> String {
    let mut out = String::new();
    // Each interstitial is spent once, the same discipline claims give
    // entities: a gap emitted for one entity cannot reappear behind another.
    let mut spent: HashSet<&str> = HashSet::new();

    // The header leads the file, not an entity: it is emitted first whatever
    // placement did with the declaration that happens to follow it.
    if let Some(header) = interstitials.merged.get("file_header") {
        out.push_str(header);
    }

    let mut first = true;
    for item in items {
        let triple = &triples[item.triple];
        let text = match &resolved[item.triple].disposition {
            Disposition::Emit { text, .. } => text.as_str(),
            Disposition::Conflict { text, .. } => text.as_str(),
            Disposition::Drop => continue,
        };
        if text.is_empty() {
            continue;
        }
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        // The gap this entity carried travels with it, byte for byte. Only a
        // boundary no version ever wrote gets a synthesised separator.
        let keys: Vec<&str> = interstitials
            .lead_keys(triple, arena)
            .into_iter()
            .filter(|k| spent.insert(k))
            .collect();
        if keys.is_empty() {
            if !first {
                out.push_str(&interstitials.separator);
            }
        } else {
            for key in keys {
                if let Some(gap) = interstitials.merged.get(key) {
                    out.push_str(gap);
                    if !gap.ends_with('\n') {
                        out.push('\n');
                    }
                }
            }
        }
        out.push_str(text);
        if !text.ends_with('\n') {
            out.push('\n');
        }
        first = false;
    }

    // Gaps nothing surviving leads, then the footer. Both are text some version
    // wrote; neither is this merge's to discard.
    for key in &interstitials.trailing {
        if !spent.insert(key.as_str()) {
            continue;
        }
        if let Some(gap) = interstitials.merged.get(key.as_str()) {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(gap);
        }
    }
    if let Some(footer) = interstitials.merged.get("file_footer") {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(footer);
    }
    if let Some(only) = interstitials.merged.get("file_only") {
        if items.is_empty() {
            out.push_str(only);
        }
    }
    out
}
