//! The def/use stage: name-resolution failure checks, at **every** scope.
//!
//! Two changes can touch different entity keys and still fail to merge, because
//! one references a name the other defines. This stage checks the def/use
//! failures weave can decide from the entity model: dangling references,
//! duplication, shadowing, and ordering.
//!
//! v1 checked it at one scope (module) and for one shape (a call to a
//! top-level def). Two cells escaped, and both are closed here:
//!
//!   * **LOCAL-DANGLING** — the same case one scope down. Ours extracts a
//!     local assignment into a helper; theirs adds a use of that local. The
//!     hunks are disjoint and non-adjacent, so diff3 merges them clean, and the
//!     result reads a name nothing assigns. Neither side wrote that program.
//!   * **REHOME-DUP** — the DUPLICATE class for *relational* edits. A body that
//!     moved house is still the same body: if it also survives elsewhere, or if
//!     the two sides moved it to different houses, the output states it twice.
//!     The per-key table cannot see this; the matcher's relational evidence
//!     can.
//!
//! Authority: bind may turn a disposition into a conflict, may rewrite call
//! sites inside a disposition it can justify, and — only under a read/write set it
//! has computed — may turn a CONFLICT into a composition. It may not reorder,
//! re-pair, or invent text.
//!
//! That third power is [`footprint_license`], and it is the same evidence read
//! the other way round. Every other pass here uses def/use edges to *refuse* a
//! merge diff3 was happy with; this one uses their absence to *permit* one diff3
//! refused. It only runs on dispositions already marked conflicts, so it can
//! turn a conflict into a clean merge but never the reverse.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use super::resolve::Resolved;
use super::types::*;
use crate::binding::{
    called_names, has_call_reference, is_definition_line, replace_at_word_boundaries,
};
use crate::conflict::{classify_conflict, ConflictKind, EntityConflict, MarkerFormat};
use crate::merge::ResolutionStrategy;
use crate::validate::{SemanticWarning, WarningKind};

pub(crate) struct BindCtx<'a> {
    pub marker_format: &'a MarkerFormat,
    pub file_path: &'a str,
    /// The same context `resolve` ran the ladder with. `footprint_license` re-
    /// runs the ladder's two composing leaves — the container merge and the
    /// statement fold — with the license granted, and it must run them on
    /// exactly the inputs resolve did or it is answering a different question.
    pub resolve: &'a super::resolve::ResolveCtx<'a>,
}

/// What `bind` learned that the finished dispositions do not say for
/// themselves: how many call sites it rewrote, and every composition it
/// licensed, with the evidence.
pub(crate) struct BindReport {
    pub references_rewritten: usize,
    pub findings: Vec<SemanticWarning>,
}

/// Run the binding passes. Returns the number of call sites rewritten — the
/// one thing bind knows that the finished dispositions do not say for
/// themselves. A return value, not a `&mut` counter: bind has no business
/// holding a handle to the merge summary, and the summary is folded from the
/// dispositions afterwards by the stage that owns the result.
#[must_use]
pub(crate) fn bind(
    arena: &Arena,
    triples: &[Triple],
    cells: &[Cell],
    relational: &[Relational],
    resolved: &mut [Resolved],
    ctx: &BindCtx<'_>,
) -> BindReport {
    let references_rewritten = rename_repair(arena, triples, resolved);
    delete_vs_new_caller(arena, triples, cells, resolved, ctx);
    shadowed_binding(arena, triples, cells, resolved, ctx);
    local_dangling(arena, triples, resolved, ctx);
    rehome_duplication(arena, triples, relational, resolved, ctx);
    extracted_edit_landing(arena, triples, relational, resolved, ctx);
    contested_extraction(arena, triples, relational, resolved, ctx);
    // Last, so every refusal above has already been made: the license is only
    // ever granted over a conflict that has survived the whole binding stage.
    let findings = footprint_license(arena, triples, resolved, ctx);
    BindReport {
        references_rewritten,
        findings,
    }
}

// ---------------------------------------------------------------------------
// Disjoint composition, as a license
// ---------------------------------------------------------------------------

/// Resolve a conflict the merge ladder refused, when — and only when — the two
/// sides' def/use read/write sets are disjoint at every scope the edit touched.
///
/// The other passes in this module are alarms: they read def/use edges and turn
/// a clean disposition into a conflict. The evidence reads the other way round
/// just as well. If ours writes nothing theirs reads or writes, and theirs
/// writes nothing ours reads or writes, then applying both edits is safe here,
/// because neither writes a name the other reads or writes.
/// Refusing a composition under that hypothesis is not caution, it is
/// a conflict marker with nothing behind it.
///
/// Three things keep the license honest:
///
/// 1. **It reaches only conflicts.** `resolve` has already run the whole ladder
///    unlicensed; a triple is here only because diff3, the decorator merge, the
///    container merge and the statement fold all refused it. So the pass can
///    turn CONFLICT into CLEAN and can do nothing else — that it only moves
///    conflicts is a fact about which dispositions it looks at, not a measurement.
/// 2. **The read/write set is computed, not assumed.** `binding::footprint` answers
///    `None` for any statement whose effect it cannot see — a bare call, a
///    `return`, an `if`, an augmented assignment, a decorated definition — so
///    the disjointness test never runs on evidence that is not there.
/// 3. **Every licensed resolution is on the findings channel**, carrying the
///    names each side's block binds. A resolution whose reason nobody can see is
///    a guess with better manners.
///
/// The no-loss backstops are unchanged and still inside the fold: a licensed
/// composition that drops a unanimous line or fabricates one is refused there,
/// before this pass ever sees it.
fn footprint_license(
    arena: &Arena,
    triples: &[Triple],
    resolved: &mut [Resolved],
    ctx: &BindCtx<'_>,
) -> Vec<SemanticWarning> {
    let mut findings = Vec::new();
    for i in 0..triples.len() {
        if !matches!(
            resolved[i].strategy,
            ResolutionStrategy::ConflictBothModified
                | ResolutionStrategy::InnerMerged
                | ResolutionStrategy::ConflictStatementScoped
        ) {
            continue;
        }
        if !resolved[i].disposition.is_conflict() {
            continue;
        }
        let (Some(b), Some(o), Some(t)) = (
            triples[i].base_idx(),
            triples[i].ours_idx(),
            triples[i].theirs_idx(),
        ) else {
            continue;
        };
        let (be, oe, te) = (arena.get(b), arena.get(o), arena.get(t));
        // A container's members are reached through the container merge; a
        // plain body through the fold. Same license, two doors.
        let attempt = if crate::merge::is_container_entity_type(be.entity_type()) {
            let mut evidence = Vec::new();
            super::resolve::inner_merge(
                arena,
                &triples[i],
                ctx.resolve,
                &be.content,
                &oe.content,
                &te.content,
                crate::statement::License::Footprint,
                &mut evidence,
            )
            .map(|r| (r, evidence))
        } else {
            crate::statement::statement_merge_licensed(
                &be.content,
                &oe.content,
                &te.content,
                &crate::merge::ScopeMarkers::inside(
                    ctx.marker_format,
                    be.entity_type(),
                    be.name(),
                    "statement_fold",
                ),
            )
        };
        let Some((merged, evidence)) = attempt else {
            continue;
        };
        // A conflicted fold is the conflict this triple already has, and an
        // answer the license did not pay for is one `resolve` would have taken
        // already — taking it here would be a second, silently different route
        // to the same cell.
        if merged.has_conflicts || evidence.is_empty() {
            continue;
        }
        let name = be.name().to_string();
        let entity_type = be.entity_type().to_string();
        for gap in evidence {
            findings.push(SemanticWarning {
                entity_name: name.clone(),
                entity_type: entity_type.clone(),
                file_path: ctx.file_path.to_string(),
                kind: WarningKind::CompositionLicensed {
                    ours_binds: gap.ours_binds,
                    theirs_binds: gap.theirs_binds,
                },
                related: Vec::new(),
            });
        }
        resolved[i] = Resolved::revised(
            Disposition::Emit {
                text: merged.content,
                name,
            },
            ResolutionStrategy::FootprintLicensed,
            Tally::BothMerged,
        );
    }
    findings
}

// ---------------------------------------------------------------------------
// RENAME — the other side's call sites follow the definition
// ---------------------------------------------------------------------------

/// One side renamed `X` to `Y`; the other side's code still calls `X`. If
/// nothing in the merged output defines `X` any more, every such call is
/// dangling, and rewriting it is the only reading that keeps both sides' work.
///
/// The condition is stated over the *output*, not over which side did what,
/// which is why it cannot acquire a direction dependence — the same repair
/// fires whichever side did the renaming.
fn rename_repair(arena: &Arena, triples: &[Triple], resolved: &mut [Resolved]) -> usize {
    let mut rewritten = 0usize;
    let pending: Vec<(usize, String, String)> = resolved
        .iter()
        .enumerate()
        .filter_map(|(i, r)| r.rename().map(|(old, new)| (i, old.clone(), new.clone())))
        .collect();
    if pending.is_empty() {
        return rewritten;
    }
    let surviving_names: HashSet<String> = surviving_definitions(arena, triples, resolved);
    // Which dispositions call which names, indexed once. Asking every
    // disposition about every rename is |renames| x |entities| substring scans
    // — the shape that made a 2000-entity rename refactor slow enough to need a
    // watchdog thread. One pass over the texts answers it instead.
    let mut callers: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, r) in resolved.iter().enumerate() {
        if let Disposition::Emit { text, .. } = &r.disposition {
            for name in called_names(text) {
                callers.entry(name).or_default().push(i);
            }
        }
    }
    let callers: HashMap<String, Vec<usize>> = callers
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    for (owner, old, new) in pending {
        if old.is_empty() || new.is_empty() || old == new {
            continue;
        }
        // Someone still defines the old name — the calls are not dangling and
        // rewriting them would silently rebind (a SHADOW, not a repair).
        if surviving_names.contains(&old) {
            continue;
        }
        for &i in callers.get(&old).map(Vec::as_slice).unwrap_or(&[]) {
            if i == owner {
                continue;
            }
            let Disposition::Emit { text, .. } = &mut resolved[i].disposition else {
                continue;
            };
            if !has_call_reference(text, &old) {
                continue;
            }
            *text = replace_at_word_boundaries(text, &old, &new);
            rewritten += 1;
        }
    }
    rewritten
}

/// Every name the merged output still defines.
fn surviving_definitions(
    arena: &Arena,
    triples: &[Triple],
    resolved: &[Resolved],
) -> HashSet<String> {
    let mut out = HashSet::new();
    for (t, r) in triples.iter().zip(resolved.iter()) {
        match &r.disposition {
            Disposition::Emit { name, .. } => {
                out.insert(name.clone());
            }
            Disposition::Conflict { .. } => {
                // A conflict keeps both sides' text in the artifact, so both
                // names are still written down.
                for idx in [t.ours_idx(), t.theirs_idx()].into_iter().flatten() {
                    out.insert(arena.get(idx).name().to_string());
                }
            }
            Disposition::Drop => {}
        }
    }
    out
}

// ---------------------------------------------------------------------------
// DANGLING — a deletion the other side's new code depends on
// ---------------------------------------------------------------------------

fn delete_vs_new_caller(
    arena: &Arena,
    triples: &[Triple],
    cells: &[Cell],
    resolved: &mut [Resolved],
    ctx: &BindCtx<'_>,
) {
    let added_texts = added_entity_texts(arena, triples, cells, resolved);
    if added_texts.is_empty() {
        return;
    }
    let surviving = surviving_definitions(arena, triples, resolved);
    for i in 0..triples.len() {
        if !matches!(resolved[i].disposition, Disposition::Drop) {
            continue;
        }
        let Some(base_idx) = triples[i].base_idx() else {
            continue;
        };
        let base_e = arena.get(base_idx);
        let name = base_e.name().to_string();
        if name.is_empty() || surviving.contains(&name) {
            continue;
        }
        // Only a call site added by the side that did NOT delete matters: its
        // author never saw the deletion, and the deleter never saw the call. A
        // side that deletes something and then calls it has written its own
        // bug, and the merge is reporting it faithfully.
        let deleter = deleting_sides(cells[i]);
        let caller = added_texts
            .iter()
            .find(|(side, text)| !deleter.contains(side) && has_call_reference(text, &name));
        let Some((caller_side, _)) = caller else {
            continue;
        };
        // `modified_in_ours` names the side that KEPT the entity — the same
        // side that added the caller.
        let survivor = *caller_side;
        let base_rc = Some(base_e.content.clone());
        let survivor_rc = triples[i]
            .side_idx(survivor)
            .map(|x| arena.get(x).content.clone());
        let modified_in_ours = survivor == Side::Ours;
        let (ours_content, theirs_content) = match survivor {
            Side::Ours => (survivor_rc, None),
            Side::Theirs => (None, survivor_rc),
        };
        let complexity = classify_conflict(
            base_rc.as_deref(),
            ours_content.as_deref(),
            theirs_content.as_deref(),
        );
        let conflict = EntityConflict {
            entity_name: name,
            entity_type: base_e.entity_type().to_string(),
            kind: ConflictKind::ModifyDelete { modified_in_ours },
            complexity,
            ours_content,
            theirs_content,
            base_content: base_rc,
        };
        become_conflict(
            &mut resolved[i],
            conflict,
            ctx,
            ResolutionStrategy::ConflictModifyDelete,
        );
    }
}

/// Which sides removed the entity in this cell — the sides whose *additions*
/// could be depending on a definition that is now gone.
fn deleting_sides(cell: Cell) -> Vec<Side> {
    match cell {
        Cell::DeletedBoth => vec![Side::Ours, Side::Theirs],
        Cell::DeleteVsUnchanged { deleter }
        | Cell::DeleteVsEdit { deleter }
        | Cell::DeleteVsRename { deleter }
        | Cell::DeleteVsRenameEdit { deleter } => vec![deleter],
        _ => vec![],
    }
}

/// The text each side ADDED outright, tagged with the side that added it.
fn added_entity_texts(
    arena: &Arena,
    triples: &[Triple],
    cells: &[Cell],
    resolved: &[Resolved],
) -> Vec<(Side, String)> {
    let mut out = Vec::new();
    for ((t, cell), _r) in triples.iter().zip(cells.iter()).zip(resolved.iter()) {
        match cell {
            Cell::AddedOneSide { adder } => {
                if let Some(idx) = t.side_idx(*adder) {
                    out.push((*adder, arena.get(idx).content.clone()));
                }
            }
            Cell::AddedBothConvergent | Cell::AddedBothDivergent => {
                for side in [Side::Ours, Side::Theirs] {
                    if let Some(idx) = t.side_idx(side) {
                        out.push((side, arena.get(idx).content.clone()));
                    }
                }
            }
            _ => {}
        }
    }
    out
}

// ---------------------------------------------------------------------------
// SHADOW — a new definition captures a name the other side was using
// ---------------------------------------------------------------------------

fn shadowed_binding(
    arena: &Arena,
    triples: &[Triple],
    cells: &[Cell],
    resolved: &mut [Resolved],
    ctx: &BindCtx<'_>,
) {
    let added_texts = added_entity_texts(arena, triples, cells, resolved);
    if added_texts.is_empty() {
        return;
    }
    let base_names: HashSet<&str> = triples
        .iter()
        .filter_map(|t| t.base_idx())
        .map(|i| arena.get(i).name())
        .collect();
    for i in 0..triples.len() {
        let Cell::AddedOneSide { adder } = cells[i] else {
            continue;
        };
        let Some(idx) = triples[i].side_idx(adder) else {
            continue;
        };
        let name = arena.get(idx).name().to_string();
        if name.is_empty() || base_names.contains(name.as_str()) {
            continue;
        }
        // The other side's NEW code called this name, so it was resolving to
        // something else — an import, a global — and this definition captures
        // it. Neither side wrote the rebound program.
        let captured = added_texts
            .iter()
            .any(|(side, text)| *side != adder && has_call_reference(text, &name));
        if !captured || !matches!(resolved[i].disposition, Disposition::Emit { .. }) {
            continue;
        }
        let text = arena.get(idx).content.clone();
        let (ours_content, theirs_content) = match adder {
            Side::Ours => (Some(text), None),
            Side::Theirs => (None, Some(text)),
        };
        let complexity =
            classify_conflict(None, ours_content.as_deref(), theirs_content.as_deref());
        let conflict = EntityConflict {
            entity_name: name,
            entity_type: arena.get(idx).entity_type().to_string(),
            kind: ConflictKind::BothAdded,
            complexity,
            ours_content,
            theirs_content,
            base_content: None,
        };
        become_conflict(
            &mut resolved[i],
            conflict,
            ctx,
            ResolutionStrategy::ConflictBothAdded,
        );
    }
}

// ---------------------------------------------------------------------------
// LOCAL-DANGLING — the dangling class, one scope down
// ---------------------------------------------------------------------------

/// A local that base assigned inside this entity, which the merged body still
/// *uses* but no longer *assigns*.
///
/// Only fires on a body neither side wrote alone (a diff3/decorator/inner
/// merge). If one side's own text reads a name it does not assign, that is that
/// side's bug and the merge is faithfully reporting it; conflicting there would
/// be inventing a complaint. The merge is only responsible for programs it
/// composed.
///
/// And only ever of a body that IS one scope. "Assigned somewhere in base, not
/// assigned anywhere in the output, mentioned somewhere in the output" is a
/// statement about a single run of code; a class body is a dozen independent
/// scopes that happen to share a text, and the commonest local names — `i`,
/// `message`, `state`, `entry`, `expr` — recur across them. Asking the question
/// at container granularity produces false conflicts and could not do
/// otherwise: the premise was false before the evidence was gathered.
///
/// So a container is not exempt from the disjointness check — it is asked the question
/// once per MEMBER. `container::member_texts` gives the same
/// indentation reading of the base body and of the merged body, the members are
/// paired by name-and-ordinal, and each pair is one scope, which is the shape
/// the check was proved for. A member the merge dropped or renamed is not asked
/// about: its locals went with it. A body that will not decompose is skipped,
/// as before — the premise is unavailable, not false.
fn local_dangling(arena: &Arena, triples: &[Triple], resolved: &mut [Resolved], ctx: &BindCtx<'_>) {
    for i in 0..triples.len() {
        if !composed_by_the_merge(&resolved[i].strategy) {
            continue;
        }
        let Disposition::Emit { text, .. } = &resolved[i].disposition else {
            continue;
        };
        let Some(base_idx) = triples[i].base_idx() else {
            continue;
        };
        let base_e = arena.get(base_idx);
        let dangling: Vec<String> = if crate::merge::is_container_entity_type(base_e.entity_type())
        {
            let (Some(base_members), Some(out_members)) = (
                crate::container::member_texts(&base_e.content),
                crate::container::member_texts(text),
            ) else {
                continue;
            };
            let out_by_key: HashMap<_, _> =
                out_members.iter().map(|(k, t)| (k, t.as_str())).collect();
            // A member does not only see its own locals. A field of the
            // class is in scope in every method of it, and a common shape
            // is a base method that shadows a field with a local of the
            // same name (`Util.Status result = status`
            // over `private Result result`). Reading the merged member
            // alone, the field access left behind looks like a use with no
            // binding. So the container's own bindings are added to the
            // answer side — which can only retire an alarm, never raise
            // one, and leaves the question itself per-member.
            let mut container_bound = bound_locals(text);
            // A member's NAME is bound in every sibling's scope: a field
            // declared `private File spoolDirectory;` binds nothing an
            // assignment scan can see, but every method of the class reads
            // it. The member list already knows those names.
            container_bound.extend(out_members.iter().map(|(k, _)| k.0.clone()));
            let mut found: Vec<String> = Vec::new();
            for (key, base_member) in &base_members {
                // A member the indentation reading could not name — an
                // anonymous block, a struct literal — is identified only by
                // its ordinal, and an ordinal shifts when a sibling is
                // added. Pairing on it compares two unrelated scopes, which
                // is the very thing per-member scoping exists to stop.
                if !key.0.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                    || IS_NOT_A_MEMBER_NAME.contains(&key.0.as_str())
                {
                    continue;
                }
                let Some(out_member) = out_by_key.get(key) else {
                    continue;
                };
                found.extend(
                    scope_dangling(base_member, out_member)
                        .into_iter()
                        .filter(|n| !container_bound.contains(n)),
                );
            }
            found.sort();
            found.dedup();
            found
        } else {
            scope_dangling(&base_e.content, text)
        };
        if dangling.is_empty() {
            continue;
        }
        let ours_rc = triples[i].ours_idx().map(|x| arena.get(x).content.clone());
        let theirs_rc = triples[i]
            .theirs_idx()
            .map(|x| arena.get(x).content.clone());
        let complexity = classify_conflict(
            Some(&base_e.content),
            ours_rc.as_deref(),
            theirs_rc.as_deref(),
        );
        let conflict = EntityConflict {
            entity_name: base_e.name().to_string(),
            entity_type: base_e.entity_type().to_string(),
            kind: ConflictKind::BothModified,
            complexity,
            ours_content: ours_rc,
            theirs_content: theirs_rc,
            base_content: Some(base_e.content.clone()),
        };
        become_conflict(
            &mut resolved[i],
            conflict,
            ctx,
            ResolutionStrategy::ConflictBothModified,
        );
    }
}

/// Names `extract_member_name` yields when the member has no name it could
/// read — a nested type declaration, most often. A member that is itself a
/// container is many scopes, not one, which is the same reason the check does
/// not ask its question of a container.
const IS_NOT_A_MEMBER_NAME: &[&str] = &[
    "class",
    "interface",
    "enum",
    "record",
    "struct",
    "impl",
    "trait",
    "namespace",
    "module",
    "static",
    "new",
    "return",
    "if",
    "for",
    "while",
    "switch",
    "try",
];

/// The disjointness question, asked of ONE scope: names `base` assigns that
/// `out` still reads and no longer assigns.
fn scope_dangling(base: &str, out: &str) -> Vec<String> {
    let base_assigned = assigned_locals(base);
    if base_assigned.is_empty() {
        return Vec::new();
    }
    let out_assigned = bound_locals(out);
    base_assigned
        .difference(&out_assigned)
        .filter(|n| uses_local(out, n))
        .cloned()
        .collect()
}

fn composed_by_the_merge(strategy: &ResolutionStrategy) -> bool {
    matches!(
        strategy,
        ResolutionStrategy::DiffyMerged
            | ResolutionStrategy::DecoratorMerged
            | ResolutionStrategy::InnerMerged
            | ResolutionStrategy::StatementMerged
    )
}

/// Names this text assigns at statement level: `x = …`, `let x = …`,
/// `const x = …`, `var x = …`. Comparisons (`==`, `>=`, …) are excluded.
fn assigned_locals(content: &str) -> BTreeSet<String> {
    assignments(content, false)
}

/// The same reading, widened by the declaration forms of a typed language:
/// `DataInputStream dis = new …` binds `dis` exactly as the bare `dis = new …`
/// on the next line does.
///
/// It is used for the OUTPUT side only, and the asymmetry is the point. The
/// question is "does the merged text still bind this name", and recognising
/// more binding forms in the answer can only retire an alarm, never raise one.
/// Reading the wide form on the BASE side too would do the opposite: it would
/// put every `int i` in every `for` header into the set of names the check
/// then demands the output still bind, turning clean loop-header edits that
/// never touched the body into false conflicts.
fn bound_locals(content: &str) -> BTreeSet<String> {
    let mut out = assignments(content, true);
    out.extend(signature_names(content));
    out
}

/// Every identifier in the declaration's parameter list.
///
/// A parameter binds a name, and a body that rebinds it (`to_replace =
/// self._try_cast(to_replace)`) and then loses that rebinding to the other
/// side's rewrite has lost nothing — the parameter is still there. The scan
/// is deliberately generous: it takes type
/// names and defaults along with the parameters, because everything it adds
/// can only retire an alarm.
fn signature_names(content: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut depth = 0i32;
    let mut started = false;
    for line in content.lines().take(12) {
        for c in line.chars() {
            match c {
                '(' | '[' => {
                    depth += 1;
                    started = true;
                }
                ')' | ']' => depth -= 1,
                _ => {}
            }
        }
        for tok in line.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
            if !tok.is_empty() && !tok.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                out.insert(tok.to_string());
            }
        }
        if started && depth <= 0 {
            break;
        }
    }
    out
}

fn assignments(content: &str, typed_declarations: bool) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    // Bracket depth carried across lines. An `=` inside an open bracket is a
    // keyword ARGUMENT, not an assignment — `DataFrame.from_dict(dict(\n
    // freqs=[3/8.], …))` binds nothing, and reading it as a binding would make
    // the disjointness check demand that the merged body still "assign" `freqs`.
    // The gate is on the strict reading only: it can only take names OUT of
    // the question, never out of the answer, so a miscount from a bracket
    // inside a string literal cannot manufacture an alarm.
    let mut depth: i32 = 0;
    for line in content.lines() {
        let mut t = line.trim();
        let opened = depth;
        // Only call and subscript brackets. A brace is a BLOCK in every
        // language here, so counting it would put every statement of every
        // C-family body at depth 1 and the gate would swallow the whole check.
        depth += line.chars().fold(0i32, |d, c| match c {
            '(' | '[' => d + 1,
            ')' | ']' => d - 1,
            _ => d,
        });
        depth = depth.max(0);
        if t.is_empty() || t.starts_with('#') || t.starts_with("//") {
            continue;
        }
        if !typed_declarations && (opened > 0 || t.ends_with(',')) {
            continue;
        }
        for kw in ["let ", "const ", "var "] {
            if let Some(rest) = t.strip_prefix(kw) {
                t = rest.trim_start();
            }
        }
        let Some(eq) = t.find('=') else { continue };
        // `==`, `!=`, `<=`, `>=` are not assignments; `x += 1` reads x first.
        if t[eq..].starts_with("==") {
            continue;
        }
        let prev = t[..eq].chars().next_back();
        if matches!(
            prev,
            Some('!') | Some('<') | Some('>') | Some('+') | Some('-')
        ) {
            continue;
        }
        let lhs = t[..eq].trim();
        // A bare identifier only — destructuring and attribute assignment bind
        // elsewhere and are outside this check's remit. Under
        // `typed_declarations` the type in front of the name is skipped too.
        let name = if typed_declarations {
            lhs.rsplit(char::is_whitespace).next().unwrap_or(lhs)
        } else {
            lhs
        };
        if !name.is_empty()
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && !name.chars().next().is_some_and(|c| c.is_ascii_digit())
        {
            out.insert(name.to_string());
        }
    }
    out
}

/// Does this text read `name` anywhere other than on an assignment's left side?
fn uses_local(content: &str, name: &str) -> bool {
    content.lines().any(|line| {
        let t = line.trim();
        if is_definition_line(line, name) {
            return false;
        }
        // Skip the assignment line itself.
        if let Some(eq) = t.find('=') {
            if t[..eq].trim() == name {
                return false;
            }
        }
        let marked = replace_at_word_boundaries(t, name, "\u{0}");
        // A field access spelled like the local is not the local: `existing
        // ?.terminalCommandId` reads a property off some other object, and
        // counting it as a use let the check claim a dangling reference to a
        // name the merged text never mentions.
        marked.match_indices('\u{0}').any(|(at, _)| {
            let before = marked[..at].trim_end();
            !before.ends_with('.')
        })
    })
}

// ---------------------------------------------------------------------------
// REHOME-DUP — the DUPLICATE class for relational edits
// ---------------------------------------------------------------------------

/// A base body that moved house is still the same body. It is emitted twice
/// when the entity also survives under its own key, or when the two sides moved
/// it into *different* new homes — the case (DEL, DEL) reports as a convergent
/// deletion while every base line comes out doubled.
fn rehome_duplication(
    arena: &Arena,
    triples: &[Triple],
    relational: &[Relational],
    resolved: &mut [Resolved],
    ctx: &BindCtx<'_>,
) {
    if relational.is_empty() {
        return;
    }
    // deleted entity → the homes each side gave its body.
    let mut by_entity: BTreeMap<Idx, BTreeMap<Side, Vec<Idx>>> = BTreeMap::new();
    for r in relational {
        let Relational::RehomedInto {
            deleted,
            side,
            homes,
            ..
        } = r
        else {
            continue;
        };
        by_entity
            .entry(*deleted)
            .or_default()
            .insert(*side, homes.clone());
    }

    for (deleted, sides) in by_entity {
        let Some(i) = triples.iter().position(|t| t.base_idx() == Some(deleted)) else {
            continue;
        };
        // Homes that ARE this triple's own surviving entity are not a second
        // copy — that is a rename, already accounted for.
        let own: HashSet<Idx> = [triples[i].ours_idx(), triples[i].theirs_idx()]
            .into_iter()
            .flatten()
            .collect();
        let sides: BTreeMap<Side, Vec<Idx>> = sides
            .into_iter()
            .map(|(s, h)| (s, h.into_iter().filter(|x| !own.contains(x)).collect()))
            .filter(|(_, h): &(Side, Vec<Idx>)| !h.is_empty())
            .collect();
        if sides.is_empty() {
            continue;
        }

        let survives = matches!(resolved[i].disposition, Disposition::Emit { .. });
        let both_sides_rehomed = sides.len() == 2;
        let divergent_homes = both_sides_rehomed && {
            let texts: Vec<BTreeSet<String>> = sides
                .values()
                .map(|homes| {
                    homes
                        .iter()
                        .map(|h| arena.get(*h).content.clone())
                        .collect()
                })
                .collect();
            texts[0] != texts[1]
        };
        if !survives && !divergent_homes {
            continue;
        }
        if resolved[i].disposition.is_conflict() {
            continue;
        }

        let base_e = arena.get(deleted);
        let ours_rc = triples[i]
            .ours_idx()
            .map(|x| arena.get(x).content.clone())
            .or_else(|| sides.get(&Side::Ours).map(|h| join_homes(arena, h)));
        let theirs_rc = triples[i]
            .theirs_idx()
            .map(|x| arena.get(x).content.clone())
            .or_else(|| sides.get(&Side::Theirs).map(|h| join_homes(arena, h)));
        let complexity = classify_conflict(
            Some(&base_e.content),
            ours_rc.as_deref(),
            theirs_rc.as_deref(),
        );
        let conflict = EntityConflict {
            entity_name: base_e.name().to_string(),
            entity_type: base_e.entity_type().to_string(),
            kind: ConflictKind::BothModified,
            complexity,
            ours_content: ours_rc,
            theirs_content: theirs_rc,
            base_content: Some(base_e.content.clone()),
        };
        become_conflict(
            &mut resolved[i],
            conflict,
            ctx,
            ResolutionStrategy::ConflictBothModified,
        );
    }
}

// ---------------------------------------------------------------------------
// RELATIONAL RESOLUTION — the other side's edit follows its lines
// ---------------------------------------------------------------------------

/// One side extracted part of `X`'s body into a helper; the other side edited
/// the very lines that left. The per-key table sees EDIT×EDIT on `X` and hands
/// the whole thing to diff3, which collides the call that replaced the lines
/// with the edit that rewrote them — an honest conflict about a change that
/// composes perfectly, just not in the entity the key names.
///
/// Detection is forced by the reduction above; what awareness buys is
/// *resolution*: the edit is applied where its lines now live, and `X` takes the
/// extractor's version. This is the IntelliMerge/RefMerge move, with RefMerge's
/// posture — any ambiguity is a conflict that names both locations, never a
/// silent guess:
///
///   * the other side must have changed only lines that MOVED (an edit
///     straddling moved and staying lines has no single home);
///   * removals and additions must correspond one-for-one (otherwise which new
///     line replaces which old one is a guess);
///   * each moved line must live in exactly ONE extracted entity (a line
///     copied into two helpers has two candidate homes);
///   * both sides extracting the same source is nobody's edit to land.
fn extracted_edit_landing(
    arena: &Arena,
    triples: &[Triple],
    relational: &[Relational],
    resolved: &mut [Resolved],
    ctx: &BindCtx<'_>,
) {
    let extractions: Vec<&Relational> = relational
        .iter()
        .filter(|r| matches!(r, Relational::ExtractedInto { .. }))
        .collect();
    if extractions.is_empty() {
        return;
    }
    let triple_of_base: HashMap<Idx, usize> = triples
        .iter()
        .enumerate()
        .filter_map(|(i, t)| t.base_idx().map(|b| (b, i)))
        .collect();
    let mut triple_of_entity: HashMap<Idx, usize> = HashMap::new();
    for (i, t) in triples.iter().enumerate() {
        for idx in [t.ours_idx(), t.theirs_idx()].into_iter().flatten() {
            triple_of_entity.insert(idx, i);
        }
    }

    for ev in &extractions {
        let Relational::ExtractedInto {
            source,
            side,
            host,
            extracted,
            moved,
        } = ev
        else {
            continue;
        };
        let other = side.flip();
        // Both sides refactored the same body: there is no editor whose work
        // could be landed, and the divergence is already someone else's cell.
        if extractions.iter().any(|o| {
            matches!(o, Relational::ExtractedInto { source: s, side: sd, .. }
                     if s == source && *sd == other)
        }) {
            continue;
        }
        let Some(&ti) = triple_of_base.get(source) else {
            continue;
        };
        let Some(other_idx) = triples[ti].side_idx(other) else {
            continue; // the other side deleted it; that is a delete-vs-edit cell
        };

        let base_lines = super::match_phase::signature_lines(&arena.get(*source).content);
        let other_lines = super::match_phase::signature_lines(&arena.get(other_idx).content);
        let moved_set: BTreeSet<&String> = moved.iter().collect();
        let removed: Vec<&String> = base_lines.difference(&other_lines).collect();
        let added: Vec<&String> = other_lines.difference(&base_lines).collect();
        if removed.is_empty() || !removed.iter().any(|l| moved_set.contains(*l)) {
            continue; // the other side did not touch the lines that left
        }

        // -- the ambiguity gates ----------------------------------------
        let straddles = removed.iter().any(|l| !moved_set.contains(*l));
        let uneven = removed.len() != added.len();
        let mut targets: Vec<(usize, String, String)> = Vec::new(); // (triple, old, new)
        let mut homeless = false;
        if !straddles && !uneven {
            for (old, new) in removed.iter().zip(added.iter()) {
                let homes: Vec<Idx> = extracted
                    .iter()
                    .copied()
                    .filter(|h| {
                        super::match_phase::signature_lines(&arena.get(*h).content).contains(*old)
                    })
                    .collect();
                match (homes.len(), homes.first()) {
                    (1, Some(h)) => match triple_of_entity.get(h) {
                        Some(&hti)
                            if matches!(resolved[hti].disposition, Disposition::Emit { .. }) =>
                        {
                            targets.push((hti, (*old).clone(), (*new).clone()));
                        }
                        _ => homeless = true,
                    },
                    _ => homeless = true,
                }
            }
        }

        if straddles || uneven || homeless {
            let base_e = arena.get(*source);
            // Name both locations: the entity the key points at, and the
            // entities the body moved into. A conflict that says only "bravo"
            // sends the reader to the one place the missing half is NOT.
            let homes = extracted
                .iter()
                .map(|h| arena.get(*h).name().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            let named = format!("{} (body partly extracted into {homes})", base_e.name());
            if let Disposition::Conflict { conflict, .. } = &resolved[ti].disposition {
                // resolve already reached the honest verdict; it just could not
                // know where the other half went.
                let mut conflict = conflict.clone();
                conflict.entity_name = named;
                let strategy = resolved[ti].strategy.clone();
                become_conflict(&mut resolved[ti], conflict, ctx, strategy);
                continue;
            }
            let ours_rc = triples[ti].ours_idx().map(|x| arena.get(x).content.clone());
            let theirs_rc = triples[ti]
                .theirs_idx()
                .map(|x| arena.get(x).content.clone());
            let complexity = classify_conflict(
                Some(&base_e.content),
                ours_rc.as_deref(),
                theirs_rc.as_deref(),
            );
            let conflict = EntityConflict {
                entity_name: named,
                entity_type: base_e.entity_type().to_string(),
                kind: ConflictKind::BothModified,
                complexity,
                ours_content: ours_rc,
                theirs_content: theirs_rc,
                base_content: Some(base_e.content.clone()),
            };
            become_conflict(
                &mut resolved[ti],
                conflict,
                ctx,
                ResolutionStrategy::ConflictBothModified,
            );
            continue;
        }

        // -- land it ------------------------------------------------------
        for (hti, old, new) in targets {
            let Disposition::Emit { text, .. } = &mut resolved[hti].disposition else {
                continue;
            };
            *text = replace_normalized_line(text, &old, &new);
        }
        // The source now belongs to the extractor alone: the other side's hunk
        // has been applied elsewhere, so leaving it here too would state it
        // twice.
        let host_e = arena.get(*host);
        resolved[ti] = Resolved::revised(
            Disposition::Emit {
                text: host_e.content.clone(),
                name: host_e.name().to_string(),
            },
            match side {
                Side::Ours => ResolutionStrategy::OursOnly,
                Side::Theirs => ResolutionStrategy::TheirsOnly,
            },
            // Both sides' work is in the output, just not all of it in this
            // entity — the summary line for a composed merge.
            Tally::BothMerged,
        );
    }
}

/// Replace the line whose whitespace-normalized form is `old` with `new`,
/// keeping the target line's own indentation. Signature lines are compared
/// normalized, so the text they came from has to be found the same way.
fn replace_normalized_line(text: &str, old: &str, new: &str) -> String {
    let normalize = |l: &str| l.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let body = line.trim_end_matches('\n');
        if normalize(body) == old {
            let indent: String = body.chars().take_while(|c| c.is_whitespace()).collect();
            out.push_str(&indent);
            out.push_str(new);
            if line.ends_with('\n') {
                out.push('\n');
            }
        } else {
            out.push_str(line);
        }
    }
    out
}

fn join_homes(arena: &Arena, homes: &[Idx]) -> String {
    homes
        .iter()
        .map(|h| arena.get(*h).content.clone())
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// CONTESTED EXTRACTION — the cell nobody owned
// ---------------------------------------------------------------------------

/// Both sides pulled the SAME statements out of the same body, into different
/// new homes. One hole, two claims — so it is one conflict.
///
/// [`extracted_edit_landing`] sees this pair and steps over it, with the note
/// "the divergence is already someone else's cell". It was nobody's. The
/// per-key table pairs each helper with nothing (each is an addition on one
/// side), unions the additions, and every extracted line is emitted TWICE — once
/// in ours' helper and once in theirs'. The source entity conflicts, so a marker
/// does appear; it names the CALL that replaced the statements and says nothing
/// about the statements themselves, which sit duplicated in the frame where no
/// resolution removes them and no check used to look.
///
/// That is exactly the failure mode: a duplicated definition outside the
/// conflict markers, in a file the reader was told was conflicted, which a
/// reader has to notice and delete by hand.
///
/// The one-ruler repair: a conflict is a HOLE in the partition, and this hole is
/// bigger than one entity. The two extractions are the two claims on it, so they
/// move INSIDE the box — ours' helpers with ours' body, theirs' with theirs' —
/// and the frame is the partition minus the hole, which no longer states the
/// extracted lines at all. Both standard resolutions then yield exactly one
/// side's program.
///
/// **It reaches only dispositions that already conflict.** That is not caution,
/// it is the mission's own constraint made structural: the pass can change what
/// a conflicted file *looks like* and can change nothing about what any merge
/// *decides*. Where the source merged cleanly, two divergent extractions are
/// still a duplication — a real one, and an open one — but
/// closing it here would move verdicts, and the verdict side of this pipeline is
/// not what this mission is allowed to touch.
fn contested_extraction(
    arena: &Arena,
    triples: &[Triple],
    relational: &[Relational],
    resolved: &mut [Resolved],
    ctx: &BindCtx<'_>,
) {
    // source → side → the entities that side extracted into.
    let mut by_source: BTreeMap<Idx, BTreeMap<Side, Vec<Idx>>> = BTreeMap::new();
    for r in relational {
        if let Relational::ExtractedInto {
            source,
            side,
            extracted,
            ..
        } = r
        {
            by_source
                .entry(*source)
                .or_default()
                .insert(*side, extracted.clone());
        }
    }
    let triple_of_base: HashMap<Idx, usize> = triples
        .iter()
        .enumerate()
        .filter_map(|(i, t)| t.base_idx().map(|b| (b, i)))
        .collect();
    let mut triple_of_entity: HashMap<Idx, usize> = HashMap::new();
    for (i, t) in triples.iter().enumerate() {
        for idx in [t.ours_idx(), t.theirs_idx()].into_iter().flatten() {
            triple_of_entity.insert(idx, i);
        }
    }

    for (source, sides) in by_source {
        if sides.len() != 2 {
            continue; // only one side refactored: that is `extracted_edit_landing`
        }
        let text_of = |homes: &Vec<Idx>| -> Vec<String> {
            homes
                .iter()
                .map(|h| arena.get(*h).content.clone())
                .collect()
        };
        let ours_homes = sides.get(&Side::Ours).cloned().unwrap_or_default();
        let theirs_homes = sides.get(&Side::Theirs).cloned().unwrap_or_default();
        // Both sides extracting to the SAME text is a convergent refactoring:
        // the union states it once and there is nothing to arbitrate.
        let (ours_text, theirs_text) = (text_of(&ours_homes), text_of(&theirs_homes));
        if ours_text.iter().collect::<BTreeSet<_>>() == theirs_text.iter().collect::<BTreeSet<_>>()
        {
            continue;
        }
        let Some(&ti) = triple_of_base.get(&source) else {
            continue;
        };
        if !resolved[ti].disposition.is_conflict() {
            continue;
        }
        // Every home must be an entity this merge would otherwise EMIT on its
        // own, and must belong to a triple of its own — otherwise moving it into
        // the box would be moving text that is also somebody else's claim.
        let mut homes_to_drop: Vec<usize> = Vec::new();
        let mut ok = true;
        for h in ours_homes.iter().chain(theirs_homes.iter()) {
            match triple_of_entity.get(h) {
                Some(&hti)
                    if hti != ti
                        && matches!(resolved[hti].disposition, Disposition::Emit { .. }) =>
                {
                    homes_to_drop.push(hti)
                }
                _ => ok = false,
            }
        }
        if !ok {
            continue;
        }

        let base_e = arena.get(source);
        let with_homes = |homes: &[String], own: Option<String>| -> Option<String> {
            let mut s = String::new();
            for h in homes {
                s.push_str(h);
                if !h.ends_with('\n') {
                    s.push('\n');
                }
                s.push('\n');
            }
            s.push_str(own.as_deref()?);
            Some(s)
        };
        let ours_rc = with_homes(
            &ours_text,
            triples[ti].ours_idx().map(|x| arena.get(x).content.clone()),
        );
        let theirs_rc = with_homes(
            &theirs_text,
            triples[ti]
                .theirs_idx()
                .map(|x| arena.get(x).content.clone()),
        );
        if ours_rc.is_none() || theirs_rc.is_none() {
            continue; // one side deleted the source: a modify/delete cell, not this one
        }
        let names = |homes: &[Idx]| {
            homes
                .iter()
                .map(|h| arena.get(*h).name().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };
        let complexity = classify_conflict(
            Some(&base_e.content),
            ours_rc.as_deref(),
            theirs_rc.as_deref(),
        );
        let conflict = EntityConflict {
            entity_name: format!(
                "{} (both sides extracted its body — ours into {}, theirs into {})",
                base_e.name(),
                names(&ours_homes),
                names(&theirs_homes),
            ),
            entity_type: base_e.entity_type().to_string(),
            kind: ConflictKind::BothModified,
            complexity,
            ours_content: ours_rc,
            theirs_content: theirs_rc,
            base_content: Some(base_e.content.clone()),
        };
        become_conflict(
            &mut resolved[ti],
            conflict,
            ctx,
            ResolutionStrategy::ConflictBothModified,
        );
        // The homes are inside the box now. A byte belongs to one region of one
        // partition; leaving them in the frame as well is the duplication this
        // pass exists to close.
        for hti in homes_to_drop {
            resolved[hti] = Resolved::revised(
                Disposition::Drop,
                ResolutionStrategy::ConflictBothModified,
                Tally::Conflicted,
            );
        }
    }
}

/// Turn a decision into a conflict — the whole decision replaced in one move.
///
/// The tally is the point: bind revises what resolve decided, and when the
/// summary was a shared counter every caller had to remember which bucket
/// resolve had chosen and subtract from it. Several did not, and one
/// subtracted from a bucket resolve had not used. Replacing the tally here
/// makes the count a consequence of the revision rather than a chore attached
/// to it.
fn become_conflict(
    resolved: &mut Resolved,
    conflict: EntityConflict,
    ctx: &BindCtx<'_>,
    strategy: ResolutionStrategy,
) {
    *resolved = Resolved::revised(
        Disposition::Conflict {
            text: conflict.to_conflict_markers(ctx.marker_format, strategy.guard_or_ladder()),
            conflict,
        },
        strategy,
        Tally::Conflicted,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assignment_detection_ignores_comparisons_and_augmentation() {
        let got = assigned_locals(
            "    raw = load()\n    const value = compute(raw)\n    if raw == 1:\n    total += 1\n",
        );
        assert!(got.contains("raw"));
        assert!(got.contains("value"));
        assert_eq!(got.len(), 2, "{got:?}");
    }

    #[test]
    fn a_use_is_not_its_own_assignment() {
        assert!(!uses_local("    raw = load()\n", "raw"));
        assert!(uses_local("    audit(raw)\n", "raw"));
        assert!(!uses_local("    audit(rawest)\n", "raw"));
    }
}
